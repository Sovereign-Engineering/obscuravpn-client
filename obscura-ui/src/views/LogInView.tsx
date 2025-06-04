import { Anchor, Button, Card, CopyButton, Drawer, Group, Image, Loader, Modal, Space, Stack, Text, TextInput, Title, Transition, useComputedColorScheme } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { motion, MotionValue, useSpring, useTransform } from 'framer-motion';
import { ChangeEvent, FormEvent, ForwardedRef, forwardRef, PropsWithChildren, ReactNode, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import ExternalLinkIcon from '../components/ExternalLinkIcon';

import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_128x128.png';
import * as commands from '../bridge/commands';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import * as ObscuraAccount from '../common/accountUtils';
import { HEADER_TITLE, multiRef, normalizeError } from '../common/utils';
import DecoOrangeTop from '../res/deco/deco-orange-top.svg';
import DecoOrangeBottom from '../res/deco/deco-signup-mobile.svg';
import { fmtErrorI18n, TranslationKey } from '../translations/i18n';
import classes from './LoginView.module.css';

interface LogInProps {
  accountNumber: ObscuraAccount.AccountId,
  accountActive?: boolean
}

export default function LogIn({ accountNumber, accountActive }: LogInProps) {
  const { t } = useTranslation();
  const [loginWaiting, setLoginWaiting] = useState(false);
  const [awaitingAccountCreation, setCreatingWaiting] = useState(false);
  const [apiError, setApiError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const colorScheme = useComputedColorScheme();

  useEffect(() => {
    if (!!apiError) {
      const timeoutSeconds = apiError === 'apiSignupLimitExceeded' ? 12 * 3600 : 9;
      setTimeout(() => { setApiError(null) }, timeoutSeconds * 1000)
    }
  }, [apiError]);

  const loginErrorTimeout = useRef<number>();
  // clear timeout on component dismount
  useEffect(() => {
    return () => clearTimeout(loginErrorTimeout.current);
  }, []);

  const handleSubmit = async (e: FormEvent) => {
    // prevent refresh
    e.preventDefault();

    if (!loginWaiting && inputRef.current !== null) {
      setLoginWaiting(true);
      try {
        await commands.setInNewAccountFlow(false);
        await commands.login(ObscuraAccount.parseAccountIdInput(inputRef.current.value), true);
        loginErrorTimeout.current = window.setTimeout(() => {
          setLoginWaiting(false);
          notifications.show({
            title: t('Error'),
            message: t('loginError-unknown'),
            color: 'red'
          });
        }, 10_000);
      } catch (e) {
        const error = normalizeError(e);
        const message = error instanceof commands.CommandError
          ? fmtErrorI18n(t, error)
          : error instanceof ObscuraAccount.ObscuraAccountIdError
            ? fmtErrorI18n(t, error)
            : error.message;

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
      const error = normalizeError(e);
      if (error instanceof commands.CommandError) {
        setApiError(error.code.startsWith('api') ? error.i18nKey() : ('vpnError-' + error.message));
      } else {
        setApiError(error.message);
      }
    } finally {
      setTimeout(() => setCreatingWaiting(false), 200);
    }
  }

  return (
    <Stack h='100vh' bg={colorScheme === 'light' ? undefined : 'dark.8'} gap={20}>
      <div style={{ height: '100%', backgroundImage: `url("${IS_HANDHELD_DEVICE ? DecoOrangeBottom : DecoOrangeTop}")`, backgroundPosition: IS_HANDHELD_DEVICE ? 'bottom' : 'top' }} className={classes.backgroundImage}>
        <Space h='28vh' />
        {
          (!!accountNumber || awaitingAccountCreation) ? <AccountGeneration loading={awaitingAccountCreation} generatedAccountId={accountNumber} accountActive={accountActive} />
            :
            <Stack h='72vh' gap={20} component='form' onSubmit={handleSubmit} align='center'>
              <Group>
                <Image src={AppIcon} w={64} />
                <Title>{HEADER_TITLE}</Title>
              </Group>
              <Text size='sm' ta='center' w={300}>
                <Trans
                  i18nKey='legalNotice'
                  components={[<Anchor href={ObscuraAccount.LEGAL_WEBPAGE} />]}
                />
              </Text>
              <Button w={260} onClick={initiateAccountCreation}>{t('Create an Account')}</Button>
              {
                apiError &&
                <Card shadow='sm' padding='lg' my={0} m={0} radius='md'>
                  <Text c='red'>{t(apiError as TranslationKey)}</Text>
                </Card>
              }
              <AccountNumberInput ref={inputRef} />
              <Button w={260} disabled={loginWaiting} type='submit' variant='outline'>{loginWaiting ? <Loader size='sm' /> : t('Log In')}</Button>
            </Stack >
        }
      </div>
    </Stack >
  );
}

const SPINNING_DURATION = 900;
const ANIMATION_HEIGHT = 20;

interface AccountGenerationProps {
  generatedAccountId: ObscuraAccount.AccountId,
  accountActive?: boolean,
  loading: boolean
}

function AccountGeneration({ generatedAccountId, accountActive, loading }: AccountGenerationProps) {
  const { t } = useTranslation();
  const [value, setValue] = useState(ObscuraAccount.generateAccountNumber());
  const [confirmAccountSecured, { open, close }] = useDisclosure(false);
  const [paymentPressed, userPressOnPayment] = useState(false);
  const [copyPressed, userPressOnCopy] = useState(false);
  const timeoutRef = useRef<number>();

  const rollAccountValue = (tries: number) => {
    if (tries === 0) return setValue(generatedAccountId);
    else setValue(ObscuraAccount.generateAccountNumber())
    timeoutRef.current = window.setTimeout(() => rollAccountValue(loading ? tries : tries - 1), SPINNING_DURATION);
  }

  useEffect(() => {
    rollAccountValue(2);
    return () => clearTimeout(timeoutRef.current);
  }, [loading]);

  const showDoneButton = accountActive || paymentPressed;

  return (
    <>
      <ConfirmationDialog opened={confirmAccountSecured} onClose={close}>
        <Stack p={IS_HANDHELD_DEVICE ? 'xl' : undefined} ta={IS_HANDHELD_DEVICE ? 'center' : undefined}>
          <Text>{t('accountNumberStoredConfirmation')}</Text>
          <Anchor onClick={() => {
            userPressOnPayment(true);
            close();
          }} target='_blank' href={ObscuraAccount.payUrl(generatedAccountId)}>
            <Button>{t('Continue to payment')}</Button>
          </Anchor>
        </Stack>
      </ConfirmationDialog>
      <Stack maw={400} mx='auto' justify='center' align='center'>
        <Image src={AppIcon} w={64} />
        <AccountId accountId={value} />
        {
          <Transition mounted={value === generatedAccountId} transition='fade-up' duration={600}>
            {styles => <Stack style={styles} justify='center' align='center'>
              <CopyButton value={ObscuraAccount.accountIdToString(generatedAccountId)}>
                {({ copied, copy }) => (
                  <Button variant={copied ? 'filled' : undefined} color={copied ? 'teal' : undefined} miw={IS_HANDHELD_DEVICE ? 300 : '22ch'}
                    onClick={() => {
                      userPressOnCopy(true);
                      copy();
                    }}>
                    {copied ? t('Copied Account Number') : t('Copy Account Number')}
                  </Button>
                )}
              </CopyButton>
              <Text ta='center' fw={800} ml='xs' mr='xs'>{t('writeDownAccountNumber')}</Text>
              <Group grow={IS_HANDHELD_DEVICE} w={IS_HANDHELD_DEVICE ? 300 : undefined}>
                <Button disabled={!copyPressed} variant={IS_HANDHELD_DEVICE ? 'outline' : undefined} onClick={open} rightSection={<ExternalLinkIcon />}>{t('Payment')}</Button>
                {
                  (!IS_HANDHELD_DEVICE || showDoneButton) &&
                  <Button disabled={!showDoneButton} onClick={() => commands.setInNewAccountFlow(false)}>{t('Done')}</Button>
                }
              </Group>
            </Stack>}
          </Transition>
        }
      </Stack>
    </>
  );
}

function AccountId({ accountId }: { accountId: ObscuraAccount.AccountId }) {
  // every 4 digits, add a -
  let result = [];
  const accountIdStr = ObscuraAccount.accountIdToString(accountId);
  for (let i = 0; i < accountIdStr.length; i += 1) {
    result.push(<DigitsWheel key={i} digit={accountIdStr.charAt(i)} />)
    if (i % 4 === 3 && i !== accountIdStr.length - 1) {
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
function DigitsWheel({ digit }: { digit: string }) {
  const int = parseInt(digit);
  const mv = useSpring(int, { bounce: 0, duration: SPINNING_DURATION });

  useEffect(() => {
    mv.set(int);
  }, [mv, digit]);

  return (
    <div className={classes.digitsWheel}>
      {[...Array(10).keys()].map((i) => (
        <Digit key={i} mv={mv} number={i} />
      ))}
    </div>
  );
}

interface DigitProps {
  mv: MotionValue<number>,
  number: number
}

function Digit({ mv, number }: DigitProps) {
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

const AccountNumberInput = forwardRef(function AccountNumberInput(props: {}, ref: ForwardedRef<HTMLInputElement>) {
  // maintaining cursor index while editing is improved on top of https://stackoverflow.com/a/68928267/7732434
  const { t } = useTranslation();

  const internalRef = useRef<HTMLInputElement | null>(null);
  const [error, setError] = useState<ReactNode>();
  const [value, setValue] = useState<string>();
  const [cursorIdx, setCursorIdx] = useState<number | null>(null);

  useLayoutEffect(() => {
    const inputElem = internalRef.current;
    if (inputElem !== null) inputElem.setSelectionRange(cursorIdx, cursorIdx);
  }, [cursorIdx, value]);

  const validateAccountNumber = (value: string) => {
    try {
      ObscuraAccount.parseAccountIdInput(value);
    } catch (e) {
      const error = normalizeError(e);
      return t((error instanceof ObscuraAccount.ObscuraAccountIdError ? error.i18nKey() : error.message) as TranslationKey);
    }
    return null;
  }

  const onChange = (e: ChangeEvent<HTMLInputElement>) => {
    const newValue = ObscuraAccount.formatPartialAccountId(e.currentTarget.value);
    if (e.currentTarget.value.length === e.currentTarget.selectionStart) {
      // if appending to the value, set cursor to the end of the formatted value
      setCursorIdx(newValue.length);
    } else {
      setCursorIdx(e.currentTarget.selectionStart);
    }
    setValue(newValue);
    setError(newValue.length === 0 ? null : validateAccountNumber(e.currentTarget.value));
  }

  return <TextInput ref={multiRef(internalRef, ref)} value={value} onChange={onChange} error={error} required w={260} label={t('Obscura Account Number')} placeholder='XXXX - XXXX - XXXX - XXXX - XXXX' />
});

interface ConfirmationDialogProps extends PropsWithChildren {
  opened: boolean,
  onClose: () => void,
}


function ConfirmationDialog({ opened, onClose, children }: ConfirmationDialogProps) {
  const { t } = useTranslation();
  return (
    IS_HANDHELD_DEVICE ?
      <Drawer size='xs' radius='md' position='bottom' opened={opened} onClose={onClose} title={t('Confirmation')}>
        {children}
      </Drawer> :
      <Modal opened={opened} onClose={onClose} title={t('Confirmation')} centered>
        {children}
      </Modal>
  );
}
