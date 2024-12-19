import { Combobox, InputBase, InputBaseProps, PolymorphicComponentProps, useCombobox } from '@mantine/core';
import { useRef, useState } from 'react';

export interface Choice {
  value: string,
  text: string
}

export function SelectCreatable({ defaultValue, choices, onSubmit, inputBaseProps = {} }: { defaultValue?: string, choices: Choice[], onSubmit: (value: string) => void, inputBaseProps?: PolymorphicComponentProps<'input', InputBaseProps> }) {
  const combobox = useCombobox({
    onDropdownClose: () => combobox.resetSelectedOption(),
  });

  const filterByValue = (query?: string) => {
    return choices.filter(choice => choice.value === query);
  }

  const [data, setData] = useState(choices);
  const [value, setValue] = useState<string | null>(defaultValue || null);
  const defaultSearch = filterByValue(defaultValue)[0]?.text;
  const [search, setSearch] = useState(defaultSearch || '');

  const exactOptionMatch = data.some(item => item.text === search);
  const filteredOptions = exactOptionMatch
    ? data
    : data.filter(item => item.text.toLowerCase().includes(search.toLowerCase().trim()));

  const options = filteredOptions.map((item) => (
    <Combobox.Option value={item.value} key={item.value}>
      {item.text}
    </Combobox.Option>
  ));

  const inputRef = useRef<HTMLInputElement | null>(null);

  return (
    <Combobox
      store={combobox}
      withinPortal={false}
      onOptionSubmit={val => {
        if (val === '$create') {
          if (inputRef.current?.reportValidity()) {
            setData(current => [...current, { text: search, value: search }]);
            setValue(search);
            onSubmit(search);
          }
        } else {
          setValue(val);
          setSearch(filterByValue(val)[0]?.text || val);
          onSubmit(val);
        }
        combobox.closeDropdown();
      }}
    >
      <Combobox.Target>
        <InputBase
          ref={inputRef}
          rightSection={<Combobox.Chevron />}
          value={search}
          onChange={event => {
            combobox.openDropdown();
            combobox.updateSelectedOptionIndex();
            setSearch(event.currentTarget.value);
          }}
          onClick={() => combobox.openDropdown()}
          onFocus={() => combobox.openDropdown()}
          onBlur={() => {
            combobox.closeDropdown();
            if (value === null) {
              setSearch('');
            } else {
              const choiceAvailable = filterByValue(value)[0];
              setSearch(choiceAvailable?.text || value);
            }
          }}
          placeholder='Search value'
          rightSectionPointerEvents='none'
          {...inputBaseProps}
        />
      </Combobox.Target>

      <Combobox.Dropdown>
        <Combobox.Options>
          {options}
          {!exactOptionMatch && search.trim().length > 0 && (
            <Combobox.Option value="$create">+ {search}</Combobox.Option>
          )}
        </Combobox.Options>
      </Combobox.Dropdown>
    </Combobox>
  );
}
