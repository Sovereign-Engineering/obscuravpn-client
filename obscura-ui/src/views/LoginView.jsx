import { Anchor, BackgroundImage, Button, Card, CopyButton, Group, Image, Loader, Modal, Space, Stack, Text, TextInput, Title, Transition, useComputedColorScheme } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { motion, useSpring, useTransform } from 'framer-motion';
import { forwardRef, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { FaExternalLinkAlt } from 'react-icons/fa';

import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_128x128.png';
import * as ObscuraAccount from '../common/accountUtils';
import { HEADER_TITLE } from '../common/utils';
import DecoOrangeTop from '../res/deco/deco-orange-top.svg';
import * as commands from '../tauri/commands';
import classes from './LoginView.module.css';

export default function LogIn({ accountNumber, accountActive }) {
  const { t } = useTranslation();
  const [loginWaiting, setLoginWaiting] = useState(false);
  const [awaitingAccountCreation, setCreatingWaiting] = useState(false);
  const [apiError, setApiError] = useState(null);
  const inputRef = useRef(null);
  const colorScheme = useComputedColorScheme();

  useEffect(() => {
    if (!!apiError) {
      const timeoutSeconds = apiError === 'apiSignupLimitExceeded' ? 12 * 3600 : 9;
      setTimeout(() => { setApiError(null) }, timeoutSeconds * 1000)
    }
  }, [apiError]);

  const loginErrorTimeout = useRef();
  // clear timeout on component dismount
  useEffect(() => {
    return () => clearTimeout(loginErrorTimeout.current);
  }, []);

  const handleSubmit = async (e) => {
    // prevent refresh
    e.preventDefault();

    if (!loginWaiting) {
      setLoginWaiting(true);
      try {
        await commands.setInNewAccountFlow(false);
        await commands.login(ObscuraAccount.parseAccountIdInput(inputRef.current.value), true);
        loginErrorTimeout.current = setTimeout(() => {
          setLoginWaiting(false);
          notifications.show({
            title: t('Error'),
            message: t('loginError-unknown'),
            color: 'red'
          });
        }, 10_000);
      } catch (e) {
        let message = e instanceof commands.CommandError
          ? t(e.i18nKey())
          : e instanceof ObscuraAccount.ObscuraAccountIdError
          ? t(e.code)
          : e.message;
        notifications.show({
          title: t('Error Logging In'),
          message,
          color: 'red'
        });
        setTimeout(() => setLoginWaiting(false), 500);
      }
    }
  }

  const initiateAccountCreation = async () => {
    setCreatingWaiting(true);
    const newAccountNumber = ObscuraAccount.generateAccountNumber();
    try {
      // show new account funding flow
      await commands.setInNewAccountFlow(true);
      await commands.login(newAccountNumber, true);
    } catch (e) {
      if (e.message.startsWith('api')) {
        setApiError(e.message);
      } else {
        setApiError('vpnError-' + e.message);
      }
    } finally {
      setTimeout(() => setCreatingWaiting(false), 200);
    }
  }

  return (
    <Stack h='100vh' bg={colorScheme === 'light' ? undefined : 'dark.8'} gap={20}>
      <BackgroundImage src={DecoOrangeTop} style={{ backgroundRepeat: 'no-repeat', backgroundSize: 'contain', backgroundPosition: 'top' }}>
        <Space h='28vh' />
        {
          (!!accountNumber || awaitingAccountCreation) ? <AccountGeneration loading={awaitingAccountCreation} generatedAccountId={accountNumber} accountActive={accountActive} />
            :
            <Stack gap={20} component='form' onSubmit={handleSubmit} align='center'>
              <Group>
                <Image src={AppIcon} w={64} />
                <Title>{HEADER_TITLE}</Title>
              </Group>
              <Text component='p' size='sm'>
                <Trans
                  i18nKey='legalNotice'
                  components={[<Anchor href={ObscuraAccount.TERMS_WEBPAGE} />]}
                />
              </Text>
              <Button disabled={apiError !== null} w={260} onClick={initiateAccountCreation}>{t('Create an Account')}</Button>
              {
                apiError &&
                <Card shadow='sm' padding='lg' my={0} m={0} radius='md'>
                  <Text c='red'>{t(apiError)}</Text>
                </Card>
              }
              <AccountNumberInput ref={inputRef} />
              <Button w={260} disabled={loginWaiting} type='submit' variant='outline'>{loginWaiting ? <Loader size='sm' /> : t('Log In')}</Button>
            </Stack >
        }
      </BackgroundImage>
    </Stack >
  );
}

const SPINNING_DURATION = 900;
const ANIMATION_HEIGHT = 20;

function AccountGeneration({ generatedAccountId, accountActive, loading }) {
  const { t } = useTranslation();
  const [value, setValue] = useState(ObscuraAccount.generateAccountNumber());
  const [confirmAccountSecured, { open, close }] = useDisclosure(false);
  const [paymentClicked, userClickOnPayment] = useState(false);
  const timeoutRef = useRef(null);

  const rollAccountValue = tries => {
    if (tries === 0) return setValue(generatedAccountId);
    else setValue(ObscuraAccount.generateAccountNumber())
    timeoutRef.current = setTimeout(() => rollAccountValue(loading ? tries : tries - 1), SPINNING_DURATION);
  }

  useEffect(() => {
    rollAccountValue(2);
    return () => clearTimeout(timeoutRef.current);
  }, [loading]);

  return (
    <>
      <Modal opened={confirmAccountSecured} onClose={close} title={t('Confirmation')}>
        <Stack>
          {t('accountNumberStoredConfirmation')}
          <Anchor onClick={() => {
            userClickOnPayment(true);
            close();
          }} target='_blank' href={ObscuraAccount.payUrl(generatedAccountId)}>
            <Button>{t('Continue to payment')}</Button>
          </Anchor>
        </Stack>
      </Modal>
      <Stack maw={400} mx='auto' justify='center' align='center'>
        <Image src={AppIcon} w={64} />
        <AccountId accountId={value} />
        {
          <Transition mounted={value === generatedAccountId} transition='fade-up' duration={600}>
            {styles => <Stack style={styles} justify='center' align='center'>
              <CopyButton value={generatedAccountId}>
                {({ copied, copy }) => (
                  <Button color={copied ? 'teal' : undefined} onClick={copy}>
                    {copied ? t('Copied Account Number') : t('Copy Account Number')}
                  </Button>
                )}
              </CopyButton>
              <Text ta='center' fw={800}>{t('writeDownAccountNumber')}</Text>
              <Group>
                <Button onClick={open} rightSection={<FaExternalLinkAlt />}>{t('Payment')}</Button>
                <Button disabled={!accountActive && !paymentClicked} onClick={() => commands.setInNewAccountFlow(false)}>{t('Done')}</Button>
              </Group>
            </Stack>}
          </Transition>
        }
      </Stack>
    </>
  );
}

function AccountId({ accountId }) {
  // every 4 digits, add a -
  let result = [];
  for (let i = 0; i < accountId.length; i += 1) {
    result.push(<DigitsWheel key={i} digit={accountId.charAt(i)} />)
    if (i % 4 === 3 && i !== accountId.length - 1) {
      result.push(<span>&nbsp;-&nbsp;</span>);
    }
  }

  return (
    <Card radius='md' withBorder w={300}>
      <div className={classes.animatedAccountId}>
        {result}
      </div>
    </Card>
  );
}

// modified https://buildui.com/recipes/animated-counter
function DigitsWheel({ digit }) {
  digit = parseInt(digit);
  const mv = useSpring(digit, { bounce: 0, duration: SPINNING_DURATION });

  useEffect(() => {
    mv.set(digit);
  }, [mv, digit]);

  return (
    <div className={classes.digitsWheel}>
      {[...Array(10).keys()].map((i) => (
        <Digit key={i} mv={mv} number={i} />
      ))}
    </div>
  );
}

function Digit({ mv, number }) {
  let y = useTransform(mv, latest => {
    let placeValue = latest % 10;
    let offset = (10 + number - placeValue) % 10;

    let memo = offset * ANIMATION_HEIGHT;

    if (offset > 5) {
      memo -= 10 * ANIMATION_HEIGHT;
    }

    return memo;
  });

  return (
    <motion.span
      style={{ y }}
      className={classes.digit}
      transition={{ delay: 1 }}
    >
      {number}
    </motion.span>
  );
}

const AccountNumberInput = forwardRef(function AccountNumberInput(props, ref) {
  // maintaining cursor index while editing is improved on top of https://stackoverflow.com/a/68928267/7732434
  const { t } = useTranslation();

  const [error, setError] = useState();
  const [value, setValue] = useState();
  const [cursor, setCursor] = useState(null);

  useLayoutEffect(() => {
    const input = ref.current;
    if (input) input.setSelectionRange(cursor, cursor);
  }, [ref, cursor, value]);

  const validateAccountNumber = value => {
    try {
      ObscuraAccount.parseAccountIdInput(value);
    } catch (e) {
      return t(e.code || e.message);
    }
    return null;
  }

  const onChange = e => {
    const newValue = ObscuraAccount.formatPartialAccountId(e.currentTarget.value);
    if (e.currentTarget.value.length === e.currentTarget.selectionStart) {
      // if appending to the value, set cursor to the end of the formatted value
      setCursor(newValue.length);
    } else {
      setCursor(e.currentTarget.selectionStart);
    }
    setValue(newValue);
    setError(newValue.length === 0 ? null : validateAccountNumber(e.currentTarget.value));
  }

  return <TextInput value={value} onChange={onChange} error={error} ref={ref} required w={260} label={t('Obscura Account Number')} placeholder='XXXX - XXXX - XXXX - XXXX - XXXX' />
});
